//===============================================================================
//  #17 Morgan Le Fay 
//===============================================================================

class  WCFay extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardFayTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Faysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardFayBigTexture FILE=TEXTURES\menu\Folio\Cards\Faybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Morgan le Fay";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=17
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE120"
    skin(0)=Texture'HProps.skins.WizardCardFayTex0'
	textureBig=Texture'WizCardFayBigTexture'
	strDescriptionId="WizCard_0055"
}
