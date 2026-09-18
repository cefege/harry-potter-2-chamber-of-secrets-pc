//===============================================================================
// #45	Dunbar Oglethorpe
//===============================================================================

class  WCOglethorpe extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardOglethorpeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Oglethorpesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardOglethorpeBigTexture FILE=TEXTURES\menu\Folio\Cards\Oglethorpebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Dunbar Oglethorpe";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=45
    skin(0)=Texture'HProps.skins.WizardCardOglethorpeTex0'
	textureBig=Texture'WizCardOglethorpeBigTexture'
	strDescriptionId="WizCard_0083"
}
