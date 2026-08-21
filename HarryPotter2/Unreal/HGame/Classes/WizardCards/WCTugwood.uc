//===============================================================================
//  #90 Sacharissa Tugwood
//===============================================================================

class  WCTugwood extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardTugwoodTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Tugwoodsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardTugwoodBigTexture FILE=TEXTURES\menu\Folio\Cards\tugwoodbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Sacharissa Tugwood";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=90
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardTugwoodTex0'
	textureBig=Texture'WizCardTugwoodBigTexture'
	strDescriptionId="WizCard_0004"
}
